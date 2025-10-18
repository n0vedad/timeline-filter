#!/usr/bin/env python3
import argparse
import sys
import warnings
from typing import Optional

# Suppress known cosmetic Pydantic v2 warning emitted by atproto
try:
    from pydantic._internal._generate_schema import UnsupportedFieldAttributeWarning
    warnings.filterwarnings("ignore", category=UnsupportedFieldAttributeWarning)
except Exception:
    pass

from atproto import Client, models


def publish(user: str, password: str, name: str, description: str, server: str,
            rkey: Optional[str] = None, image: Optional[str] = None) -> None:
    try:
        client = Client()
        client.login(user, password)

        avatar_blob = None
        if image:
            try:
                with open(image, 'rb') as f:
                    avatar_blob = client.upload_blob(f.read()).blob
            except FileNotFoundError:
                print(f'Error: Image file not found: {image}', file=sys.stderr)
                sys.exit(1)
            except Exception as e:
                print(f'Error uploading image: {e}', file=sys.stderr)
                sys.exit(1)

        # Strip protocol from server hostname (did:web doesn't include protocol)
        server_hostname = server.replace('https://', '').replace('http://', '')

        record = models.AppBskyFeedGenerator.Record(
            did=f'did:web:{server_hostname}',
            display_name=name,
            description=description,
            avatar=avatar_blob,
            created_at=client.get_current_time_iso(),
        )

        if rkey:
            # Update existing record - verify it exists first
            try:
                client.com.atproto.repo.get_record(
                    models.ComAtprotoRepoGetRecord.Params(
                        repo=client.me.did,
                        collection=models.ids.AppBskyFeedGenerator,
                        rkey=rkey,
                    )
                )
            except Exception:
                print(f'Error: Feed record "{rkey}" not found. Cannot update non-existent record.', file=sys.stderr)
                print('Tip: Remove --rkey to create a new feed instead.', file=sys.stderr)
                sys.exit(1)

            response = client.com.atproto.repo.put_record(
                models.ComAtprotoRepoPutRecord.Data(
                    repo=client.me.did,
                    collection=models.ids.AppBskyFeedGenerator,
                    rkey=rkey,
                    record=record,
                )
            )
            print(f'Successfully updated feed record: {rkey}')
        else:
            # Create new record
            response = client.com.atproto.repo.create_record(
                models.ComAtprotoRepoCreateRecord.Data(
                    repo=client.me.did,
                    collection=models.ids.AppBskyFeedGenerator,
                    record=record,
                )
            )
            print('Successfully created new feed!')

        print('Feed URI :', response.uri)
    except Exception as e:
        print(f'Error publishing feed: {e}', file=sys.stderr)
        sys.exit(1)


def delete(user: str, password: str, rkey: str) -> None:
    try:
        client = Client()
        client.login(user, password)

        # First verify the record exists
        try:
            client.com.atproto.repo.get_record(
                models.ComAtprotoRepoGetRecord.Params(
                    repo=client.me.did,
                    collection=models.ids.AppBskyFeedGenerator,
                    rkey=rkey,
                )
            )
        except Exception as e:
            print(f'Error: Feed record "{rkey}" not found', file=sys.stderr)
            print(f'Details: {e}', file=sys.stderr)
            sys.exit(1)

        # Delete the record
        client.com.atproto.repo.delete_record(
            models.ComAtprotoRepoDeleteRecord.Data(
                repo=client.me.did,
                collection=models.ids.AppBskyFeedGenerator,
                rkey=rkey,
            )
        )
        print(f'Successfully deleted feed record: {rkey}')
    except Exception as e:
        print(f'Error deleting feed record: {e}', file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Publish or delete an AppBskyFeedGenerator record',
        epilog='''
Examples:
  # Create a new feed
  %(prog)s -u you.bsky.social -p APP_PASSWORD -n "My Feed" -d "Description" -s feeds.example.com

  # Update existing feed
  %(prog)s -u you.bsky.social -p APP_PASSWORD -n "Updated" -d "New desc" -s feeds.example.com -r RKEY

  # Delete a feed
  %(prog)s -u you.bsky.social -p APP_PASSWORD --delete -r RKEY
        ''',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument('-u', '--user', required=True, help='Bluesky handle (e.g., you.bsky.social)')
    parser.add_argument('-p', '--password', required=True, help='App password for the handle')
    parser.add_argument('-n', '--name', help='Feed display name (required for publish)')
    parser.add_argument('-d', '--description', help='Feed description (required for publish)')
    parser.add_argument('-i', '--image', default=None, help='Path to avatar image (optional)')
    parser.add_argument('-s', '--server', help='Feed server hostname (e.g., feeds.example.com)')
    parser.add_argument('-r', '--rkey', default=None, help='Record rkey from Feed URI (at://.../RKEY)')
    parser.add_argument('--delete', action='store_true', help='Delete the record specified by --rkey')

    args = parser.parse_args()

    # Validate user handle format
    if not args.user or '.' not in args.user:
        print('Error: Invalid user handle. Must be in format "you.bsky.social"', file=sys.stderr)
        sys.exit(2)

    # Validate password is not empty
    if not args.password or len(args.password.strip()) == 0:
        print('Error: Password cannot be empty', file=sys.stderr)
        sys.exit(2)

    if args.delete:
        # Delete mode validations
        if not args.rkey:
            print('Error: --delete requires --rkey', file=sys.stderr)
            print('Tip: Get rkey from Feed URI: at://did:plc:.../app.bsky.feed.generator/RKEY', file=sys.stderr)
            sys.exit(2)

        # Validate rkey format: [a-zA-Z0-9_~.:-]{1,512}
        # AT Protocol allows: alphanumeric, underscore, tilde, period, colon, hyphen
        import re
        if not re.match(r'^[a-zA-Z0-9_~.:-]{1,512}$', args.rkey):
            print(f'Error: Invalid rkey format: "{args.rkey}"', file=sys.stderr)
            print('Tip: rkey can contain: a-z A-Z 0-9 _ ~ . : -', file=sys.stderr)
            print('Examples: 3m3fxwkhzu42c, filtered-timeline, my:feed', file=sys.stderr)
            sys.exit(2)
        if args.rkey in ['.', '..']:
            print('Error: rkey cannot be "." or ".."', file=sys.stderr)
            sys.exit(2)

        delete(args.user, args.password, args.rkey)
        sys.exit(0)

    # Publish mode validations
    if not args.name or not args.description or not args.server:
        print('Error: publish requires --name, --description, and --server', file=sys.stderr)
        sys.exit(2)

    # Validate name is not empty
    if len(args.name.strip()) == 0:
        print('Error: Feed name cannot be empty', file=sys.stderr)
        sys.exit(2)

    # Validate description is not empty
    if len(args.description.strip()) == 0:
        print('Error: Feed description cannot be empty', file=sys.stderr)
        sys.exit(2)

    # Validate server hostname
    server_clean = args.server.replace('https://', '').replace('http://', '')
    if not server_clean or '.' not in server_clean:
        print(f'Error: Invalid server hostname: "{args.server}"', file=sys.stderr)
        print('Tip: Use format "feeds.example.com" (protocol is optional)', file=sys.stderr)
        sys.exit(2)

    # Validate rkey if provided (for update mode)
    if args.rkey and not args.rkey.isalnum():
        print(f'Error: Invalid rkey format: "{args.rkey}"', file=sys.stderr)
        print('Tip: rkey should be alphanumeric (e.g., 3m3fxwkhzu42c)', file=sys.stderr)
        sys.exit(2)

    publish(args.user, args.password, args.name, args.description, args.server, args.rkey, args.image)

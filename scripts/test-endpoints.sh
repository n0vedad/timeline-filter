#!/bin/bash
# Test Timeline Filter Endpoints
# Verifies that all required endpoints are working correctly

set -e

BASE_URL="${BASE_URL:-http://localhost:4050}"
FEED_URI="${FEED_URI:-at://did:plc:ciul6zkjqvao5uv4cpyoijdp/app.bsky.feed.generator/filtered-timeline}"

echo "==> Testing Timeline Filter Endpoints"
echo "    Base URL: $BASE_URL"
echo ""

# Test 1: Health check / Root endpoint
echo "==> Test 1: Root endpoint (health check)"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/")
if [ "$HTTP_CODE" = "200" ]; then
    echo "    ✓ PASS - Root endpoint responding ($HTTP_CODE)"
else
    echo "    ✗ FAIL - Root endpoint returned $HTTP_CODE (expected 200)"
fi
echo ""

# Test 2: describeFeedGenerator
echo "==> Test 2: describeFeedGenerator endpoint"
DESCRIBE_RESPONSE=$(curl -s "$BASE_URL/xrpc/app.bsky.feed.describeFeedGenerator")
DESCRIBE_DID=$(echo "$DESCRIBE_RESPONSE" | jq -r '.did' 2>/dev/null || echo "null")

if [ "$DESCRIBE_DID" != "null" ] && [ ! -z "$DESCRIBE_DID" ]; then
    echo "    ✓ PASS - Service DID: $DESCRIBE_DID"

    FEEDS_COUNT=$(echo "$DESCRIBE_RESPONSE" | jq '.feeds | length' 2>/dev/null || echo "0")
    echo "    ✓ Number of feeds: $FEEDS_COUNT"

    if [ "$FEEDS_COUNT" -gt 0 ]; then
        echo "    ✓ Feed URIs:"
        echo "$DESCRIBE_RESPONSE" | jq -r '.feeds[].uri' | sed 's/^/      - /'
    fi
else
    echo "    ✗ FAIL - describeFeedGenerator did not return valid DID"
    echo "    Response: $DESCRIBE_RESPONSE"
fi
echo ""

# Test 3: getFeedSkeleton
echo "==> Test 3: getFeedSkeleton endpoint"
echo "    Feed URI: $FEED_URI"

SKELETON_RESPONSE=$(curl -s "$BASE_URL/xrpc/app.bsky.feed.getFeedSkeleton?feed=$(echo -n "$FEED_URI" | jq -sRr @uri)")
SKELETON_FEED=$(echo "$SKELETON_RESPONSE" | jq -r '.feed' 2>/dev/null || echo "null")

if [ "$SKELETON_FEED" != "null" ]; then
    FEED_LENGTH=$(echo "$SKELETON_RESPONSE" | jq '.feed | length' 2>/dev/null || echo "0")
    echo "    ✓ PASS - Feed skeleton returned"
    echo "    ✓ Number of posts: $FEED_LENGTH"

    if [ "$FEED_LENGTH" -gt 0 ]; then
        echo "    ✓ Sample post URIs:"
        echo "$SKELETON_RESPONSE" | jq -r '.feed[0:3][].post' | sed 's/^/      - /'
    else
        echo "    ⚠ WARNING - Feed is empty (no posts indexed yet)"
    fi

    CURSOR=$(echo "$SKELETON_RESPONSE" | jq -r '.cursor' 2>/dev/null || echo "null")
    if [ "$CURSOR" != "null" ]; then
        echo "    ✓ Cursor present (pagination supported)"
    fi
else
    echo "    ✗ FAIL - getFeedSkeleton did not return valid feed"
    echo "    Response: $SKELETON_RESPONSE"
fi
echo ""

# Test 4: Database check
echo "==> Test 4: Database status"
if [ -f "development.db" ]; then
    POST_COUNT=$(sqlite3 development.db "SELECT COUNT(*) FROM feed_content;" 2>/dev/null || echo "0")
    USER_COUNT=$(sqlite3 development.db "SELECT COUNT(*) FROM timeline_user_config;" 2>/dev/null || echo "0")

    echo "    ✓ Database exists"
    echo "    ✓ Indexed posts: $POST_COUNT"
    echo "    ✓ Configured users: $USER_COUNT"

    if [ "$POST_COUNT" -eq 0 ]; then
        echo "    ⚠ WARNING - No posts indexed yet"
        echo "      Check if timeline consumer is running and polling"
    fi
else
    echo "    ⚠ WARNING - Database not found (development.db)"
fi
echo ""

# Test 5: Configuration check
echo "==> Test 5: Configuration check"
if [ -f "timeline_feeds.yml" ]; then
    echo "    ✓ timeline_feeds.yml exists"

    # Check if it's not the example file
    if grep -q "your-access-token-here" timeline_feeds.yml 2>/dev/null; then
        echo "    ⚠ WARNING - timeline_feeds.yml still contains example values"
        echo "      Update with your actual OAuth token"
    else
        echo "    ✓ timeline_feeds.yml appears to be configured"
    fi
else
    echo "    ✗ FAIL - timeline_feeds.yml not found"
    echo "      Run: cp timeline_feeds.example.yml timeline_feeds.yml"
fi

if [ -f ".env" ]; then
    echo "    ✓ .env exists"
else
    echo "    ⚠ WARNING - .env not found (optional)"
fi
echo ""

# Summary
echo "==> Summary"
echo ""
echo "Service Status:"
if [ "$HTTP_CODE" = "200" ] && [ "$DESCRIBE_DID" != "null" ]; then
    echo "  ✓ Service is running and responding correctly"
else
    echo "  ✗ Service has issues - check logs"
fi
echo ""

echo "Feed Status:"
if [ "$SKELETON_FEED" != "null" ] && [ "$FEED_LENGTH" -gt 0 ]; then
    echo "  ✓ Feed is working with $FEED_LENGTH posts"
elif [ "$SKELETON_FEED" != "null" ]; then
    echo "  ⚠ Feed endpoint works but no posts yet"
    echo "    Wait for timeline polling to index posts"
else
    echo "  ✗ Feed endpoint has issues"
fi
echo ""

echo "Next Steps:"
if [ "$POST_COUNT" -eq 0 ]; then
    echo "  1. Make sure service is running: ./dev-timeline-filter.sh"
    echo "  2. Wait for timeline polling (happens every 30s)"
    echo "  3. Check logs for 'Processed timeline posts' messages"
else
    echo "  1. Service is working locally! ✓"
    echo "  2. To make it visible in Bluesky app:"
    echo "     - Deploy publicly with HTTPS"
    echo "     - Setup DID document"
    echo "     - Publish feed record"
    echo "  3. See DEPLOYMENT.md for instructions"
fi
echo ""

echo "Documentation:"
echo "  - Quick Start: QUICK-START.md"
echo "  - Deployment: DEPLOYMENT.md"
echo "  - Publish Script: scripts/publish-feed.sh"
echo ""

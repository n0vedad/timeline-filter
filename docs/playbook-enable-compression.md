# Playbook: Enable compression

Jetstream supports optional zstd compression for streamed events. This feature can reduce bandwidth usage by up to 50% with minimal performance impact, but at the cost of a more complicated deployment and additional maintenance steps.

[zstd](https://github.com/facebook/zstd) is a dictionary-based compression algorithm that is optimized for real-time compression and decompression.

## Configuration

Compression is enabled by setting the `COMPRESSION` environment variable to `true`. When enabled, the `ZSTD_DICTIONARY` environment variable must be set to the path of the ZSTD dictionary to use.

The dictionary file can be downloaded from [github.com/bluesky-social/jetstream/blob/main/pkg/models/zstd_dictionary](https://github.com/bluesky-social/jetstream/blob/main/pkg/models/zstd_dictionary).

## FAQ

### Why is compression disabled by default?

The benefits of compression are not guaranteed and depend on the data being compressed. For most supercell deployments, the impact on CPU and memory is minimal, and the benefits of reduced bandwidth is significant. However, this feature is maturing and may not be suitable for all deployments.

### Why is a custom dictionary required?

Zstd uses a dictionary to improve compression performance. There is no default dictionary, so a custom dictionary must be provided.

The dictionary is occasionally rebuilt to improve compression performance. The dictionary is built from a sample of the data that is being compressed, so the dictionary is specific to the data that is being compressed.

### Dictionary mismatch

This error occurs when compression is enabled and the dictionary is invalid or does not match the dictionary that was used to compress the data.

To resolve, ensure the `ZSTD_DICTIONARY` environment variable is set to the correct path and that the file at that path is the same as the one from the jetstream repository.

### Destination buffer is too small

This error occurs when the buffer used to decompress the data is too small to contain the decompressed event.


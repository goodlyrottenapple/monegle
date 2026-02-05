// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title MonadStreamer
 * @notice Contract for streaming ASCII video frames on Monad blockchain
 * @dev Uses Execution Events SDK to read frame data from TxnData (calldata)
 *      This eliminates event emission costs - frames are sent as raw transactions
 *      and extracted via Execution Events SDK on the receiver side.
 */
contract MonadStreamer {
    /// @notice Stream metadata
    struct StreamMetadata {
        address owner;
        address streamAddress; // Dedicated address for this stream's transactions
        uint8 fps;
        uint16 width;
        uint16 height;
        uint8 compressionType;
        uint64 startTime;
        uint64 endTime;
        bool active;
    }

    /// @notice Counter for stream IDs
    uint256 public nextStreamId = 1;

    /// @notice Mapping of stream ID to metadata
    mapping(uint256 => StreamMetadata) public streams;

    /// @notice Mapping of stream address to stream ID
    mapping(address => uint256) public streamAddressToId;

    /// @notice Mapping of owner to their stream IDs
    mapping(address => uint256[]) public ownerStreams;

    /// @notice Emitted when a new stream is started
    event StreamStarted(
        uint256 indexed streamId,
        address indexed owner,
        address indexed streamAddress,
        uint8 fps,
        uint16 width,
        uint16 height,
        uint8 compressionType,
        uint64 startTime
    );

    /// @notice Emitted when a stream is ended
    event StreamEnded(
        uint256 indexed streamId,
        uint64 endTime
    );

    /// @notice Start a new stream
    /// @param streamAddress Dedicated address where frame transactions will be sent
    /// @param fps Frames per second
    /// @param width Width in characters
    /// @param height Height in characters
    /// @param compressionType Compression type (0=None, 1=Rle, 2=Delta, 3=Zlib, 4=Auto)
    /// @return streamId The ID of the newly created stream
    function startStream(
        address streamAddress,
        uint8 fps,
        uint16 width,
        uint16 height,
        uint8 compressionType
    ) external returns (uint256) {
        require(streamAddress != address(0), "Invalid stream address");
        require(streamAddressToId[streamAddress] == 0, "Stream address already in use");
        require(fps > 0 && fps <= 60, "Invalid FPS");
        require(width > 0 && height > 0, "Invalid dimensions");
        require(compressionType <= 4, "Invalid compression type");

        uint256 streamId = nextStreamId++;
        uint64 startTime = uint64(block.timestamp);

        streams[streamId] = StreamMetadata({
            owner: msg.sender,
            streamAddress: streamAddress,
            fps: fps,
            width: width,
            height: height,
            compressionType: compressionType,
            startTime: startTime,
            endTime: 0,
            active: true
        });

        streamAddressToId[streamAddress] = streamId;
        ownerStreams[msg.sender].push(streamId);

        emit StreamStarted(
            streamId,
            msg.sender,
            streamAddress,
            fps,
            width,
            height,
            compressionType,
            startTime
        );

        return streamId;
    }

    /// @notice Get stream ID from stream address
    /// @param streamAddress The stream address
    /// @return streamId The stream ID (0 if not found)
    function getStreamIdByAddress(address streamAddress) external view returns (uint256) {
        return streamAddressToId[streamAddress];
    }

    /// @notice End a stream
    /// @param streamId The stream ID to end
    function endStream(uint256 streamId) external {
        StreamMetadata storage stream = streams[streamId];
        require(stream.active, "Stream not active");
        require(msg.sender == stream.owner, "Not stream owner");

        stream.active = false;
        stream.endTime = uint64(block.timestamp);

        emit StreamEnded(streamId, stream.endTime);
    }

    /// @notice Get all stream IDs for an owner
    /// @param owner The owner address
    /// @return Array of stream IDs
    function getOwnerStreams(address owner) external view returns (uint256[] memory) {
        return ownerStreams[owner];
    }

    /// @notice Get stream metadata
    /// @param streamId The stream ID
    /// @return Stream metadata struct
    function getStream(uint256 streamId) external view returns (StreamMetadata memory) {
        return streams[streamId];
    }

    /// @notice Check if a stream is active
    /// @param streamId The stream ID
    /// @return True if stream is active
    function isStreamActive(uint256 streamId) external view returns (bool) {
        return streams[streamId].active;
    }
}

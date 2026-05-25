// SPDX-License-Identifier: AGPL-3.0
pragma solidity ^0.8.19;

/// @title HSK Transparency Anchor
/// @notice Anchors Merkle roots from the HSK consent transparency log to Ethereum
/// @dev Provides permanent, immutable verification of consent log state
contract HSKTransparencyAnchor {
    
    /// @notice Structure for anchored Merkle root
    struct Anchor {
        bytes32 merkleRoot;
        uint256 timestamp;
        uint256 blockNumber;
        bytes32 previousAnchor;
        string metadataURI;
    }
    
    /// @notice Authorized HSK oracle addresses
    mapping(address => bool) public authorizedOracles;
    
    /// @notice All anchors by sequence number
    mapping(uint256 => Anchor) public anchors;
    
    /// @notice Merkle root to sequence number mapping
    mapping(bytes32 => uint256) public rootToSequence;
    
    /// @notice Current sequence number
    uint256 public currentSequence;
    
    /// @notice Contract owner
    address public owner;
    
    /// @notice Minimum time between anchors (prevents spam)
    uint256 public constant MIN_ANCHOR_INTERVAL = 1 hours;
    
    /// @notice Last anchor timestamp
    uint256 public lastAnchorTime;
    
    /// @notice Events
    event AnchorAdded(
        uint256 indexed sequence,
        bytes32 indexed merkleRoot,
        uint256 timestamp,
        uint256 blockNumber
    );
    
    event OracleAuthorized(address indexed oracle);
    event OracleRevoked(address indexed oracle);
    
    /// @notice Modifiers
    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }
    
    modifier onlyOracle() {
        require(authorizedOracles[msg.sender], "Not authorized oracle");
        _;
    }
    
    /// @notice Constructor
    constructor() {
        owner = msg.sender;
        currentSequence = 0;
        lastAnchorTime = 0;
    }
    
    /// @notice Authorize a new oracle
    /// @param oracle Address to authorize
    function authorizeOracle(address oracle) external onlyOwner {
        require(oracle != address(0), "Invalid address");
        require(!authorizedOracles[oracle], "Already authorized");
        
        authorizedOracles[oracle] = true;
        emit OracleAuthorized(oracle);
    }
    
    /// @notice Revoke oracle authorization
    /// @param oracle Address to revoke
    function revokeOracle(address oracle) external onlyOwner {
        require(authorizedOracles[oracle], "Not authorized");
        
        authorizedOracles[oracle] = false;
        emit OracleRevoked(oracle);
    }
    
    /// @notice Anchor a new Merkle root
    /// @param merkleRoot The Merkle root to anchor
    /// @param metadataURI IPFS URI with additional metadata
    function anchorMerkleRoot(
        bytes32 merkleRoot,
        string calldata metadataURI
    ) external onlyOracle {
        require(merkleRoot != bytes32(0), "Invalid merkle root");
        require(rootToSequence[merkleRoot] == 0, "Root already anchored");
        require(
            block.timestamp >= lastAnchorTime + MIN_ANCHOR_INTERVAL,
            "Anchor interval not met"
        );
        
        currentSequence++;
        
        bytes32 previousAnchor = currentSequence > 1 
            ? anchors[currentSequence - 1].merkleRoot 
            : bytes32(0);
        
        anchors[currentSequence] = Anchor({
            merkleRoot: merkleRoot,
            timestamp: block.timestamp,
            blockNumber: block.number,
            previousAnchor: previousAnchor,
            metadataURI: metadataURI
        });
        
        rootToSequence[merkleRoot] = currentSequence;
        lastAnchorTime = block.timestamp;
        
        emit AnchorAdded(
            currentSequence,
            merkleRoot,
            block.timestamp,
            block.number
        );
    }
    
    /// @notice Batch anchor multiple Merkle roots
    /// @param merkleRoots Array of Merkle roots to anchor
    /// @param metadataURIs Array of metadata URIs
    function batchAnchorMerkleRoots(
        bytes32[] calldata merkleRoots,
        string[] calldata metadataURIs
    ) external onlyOracle {
        require(
            merkleRoots.length == metadataURIs.length,
            "Array length mismatch"
        );
        require(merkleRoots.length <= 10, "Batch too large");
        
        for (uint256 i = 0; i < merkleRoots.length; i++) {
            // Skip if already anchored
            if (rootToSequence[merkleRoots[i]] != 0) continue;
            
            currentSequence++;
            
            bytes32 previousAnchor = currentSequence > 1 
                ? anchors[currentSequence - 1].merkleRoot 
                : bytes32(0);
            
            anchors[currentSequence] = Anchor({
                merkleRoot: merkleRoots[i],
                timestamp: block.timestamp,
                blockNumber: block.number,
                previousAnchor: previousAnchor,
                metadataURI: metadataURIs[i]
            });
            
            rootToSequence[merkleRoots[i]] = currentSequence;
            
            emit AnchorAdded(
                currentSequence,
                merkleRoots[i],
                block.timestamp,
                block.number
            );
        }
        
        lastAnchorTime = block.timestamp;
    }
    
    /// @notice Verify that a Merkle root was anchored
    /// @param merkleRoot The Merkle root to verify
    /// @return wasAnchored Whether the root was anchored
    /// @return sequence The sequence number if anchored
    /// @return timestamp The anchoring timestamp
    function verifyAnchor(
        bytes32 merkleRoot
    ) external view returns (
        bool wasAnchored,
        uint256 sequence,
        uint256 timestamp
    ) {
        sequence = rootToSequence[merkleRoot];
        wasAnchored = sequence != 0;
        
        if (wasAnchored) {
            timestamp = anchors[sequence].timestamp;
        }
    }
    
    /// @notice Get the full anchor chain from a starting point
    /// @param startSequence Starting sequence number
    /// @param count Number of anchors to retrieve
    /// @return merkleRoots Array of Merkle roots
    /// @return timestamps Array of timestamps
    function getAnchorChain(
        uint256 startSequence,
        uint256 count
    ) external view returns (
        bytes32[] memory merkleRoots,
        uint256[] memory timestamps
    ) {
        require(startSequence <= currentSequence, "Invalid start sequence");
        require(count > 0 && count <= 100, "Invalid count");
        
        uint256 actualCount = count;
        if (startSequence + count - 1 > currentSequence) {
            actualCount = currentSequence - startSequence + 1;
        }
        
        merkleRoots = new bytes32[](actualCount);
        timestamps = new uint256[](actualCount);
        
        for (uint256 i = 0; i < actualCount; i++) {
            Anchor storage anchor = anchors[startSequence + i];
            merkleRoots[i] = anchor.merkleRoot;
            timestamps[i] = anchor.timestamp;
        }
    }
    
    /// @notice Verify inclusion proof against anchored Merkle root
    /// @param merkleRoot The anchored Merkle root
    /// @param leaf The leaf to verify
    /// @param proof The Merkle proof
    /// @return isValid Whether the proof is valid
    function verifyMerkleProof(
        bytes32 merkleRoot,
        bytes32 leaf,
        bytes32[] calldata proof
    ) external pure returns (bool isValid) {
        bytes32 computedHash = leaf;
        
        for (uint256 i = 0; i < proof.length; i++) {
            bytes32 proofElement = proof[i];
            
            if (computedHash <= proofElement) {
                computedHash = keccak256(
                    abi.encodePacked(computedHash, proofElement)
                );
            } else {
                computedHash = keccak256(
                    abi.encodePacked(proofElement, computedHash)
                );
            }
        }
        
        return computedHash == merkleRoot;
    }
    
    /// @notice Transfer ownership
    /// @param newOwner New owner address
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid address");
        owner = newOwner;
    }
    
    /// @notice Emergency pause (set minimum interval to max)
    function emergencyPause() external onlyOwner {
        lastAnchorTime = type(uint256).max - MIN_ANCHOR_INTERVAL;
    }
    
    /// @notice Emergency unpause
    function emergencyUnpause() external onlyOwner {
        lastAnchorTime = 0;
    }
}

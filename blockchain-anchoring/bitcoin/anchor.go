package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log"
	"time"

	"github.com/btcsuite/btcd/btcec/v2"
	"github.com/btcsuite/btcd/btcutil"
	"github.com/btcsuite/btcd/chaincfg"
	"github.com/btcsuite/btcd/chaincfg/chainhash"
	"github.com/btcsuite/btcd/txscript"
	"github.com/btcsuite/btcd/wire"
	"github.com/btcsuite/btcwallet/wallet/txauthor"
)

// BitcoinAnchor anchors HSK Merkle roots to the Bitcoin blockchain
// using OP_RETURN transactions for permanent, immutable storage.

type BitcoinAnchor struct {
	privateKey *btcec.PrivateKey
	address    btcutil.Address
	network    *chaincfg.Params
	client     BitcoinRPCClient
}

// BitcoinRPCClient interface for Bitcoin node RPC
type BitcoinRPCClient interface {
	GetRawChangeAddress() (btcutil.Address, error)
	GetUnspent(address btcutil.Address) ([]*UTXO, error)
	SendRawTransaction(tx *wire.MsgTx) (*chainhash.Hash, error)
	GetTransaction(txid *chainhash.Hash) (*Transaction, error)
}

// UTXO represents an unspent transaction output
type UTXO struct {
	TxID   string
	Vout   uint32
	Amount int64
}

// Transaction represents a Bitcoin transaction
type Transaction struct {
	TxID          string
	Confirmations int64
	BlockHash     string
	BlockTime     int64
}

// AnchorRecord represents an anchored Merkle root
type AnchorRecord struct {
	MerkleRoot    [32]byte  `json:"merkle_root"`
	Timestamp     int64     `json:"timestamp"`
	TxID          string    `json:"txid"`
	BlockHeight   int64     `json:"block_height"`
	BlockHash     string    `json:"block_hash"`
	OPReturnData  []byte    `json:"op_return_data"`
}

// NewBitcoinAnchor creates a new Bitcoin anchor instance
func NewBitcoinAnchor(
	wif string,
	network *chaincfg.Params,
	client BitcoinRPCClient,
) (*BitcoinAnchor, error) {
	// Decode WIF private key
	wifKey, err := btcutil.DecodeWIF(wif)
	if err != nil {
		return nil, fmt.Errorf("failed to decode WIF: %w", err)
	}

	// Generate address from private key
	pubKey := wifKey.PrivKey.PubKey()
	pubKeyHash := btcutil.Hash160(pubKey.SerializeCompressed())
	address, err := btcutil.NewAddressPubKeyHash(pubKeyHash, network)
	if err != nil {
		return nil, fmt.Errorf("failed to create address: %w", err)
	}

	return &BitcoinAnchor{
		privateKey: wifKey.PrivKey,
		address:    address,
		network:    network,
		client:     client,
	}, nil
}

// AnchorMerkleRoot anchors a Merkle root to Bitcoin via OP_RETURN
func (ba *BitcoinAnchor) AnchorMerkleRoot(merkleRoot [32]byte) (*AnchorRecord, error) {
	// Build OP_RETURN data
	// Format: HSK + 2 bytes version + 32 bytes Merkle root = 36 bytes
	opReturnData := buildOPReturnData(merkleRoot)

	// Create OP_RETURN script
	opReturnScript, err := txscript.NewScriptBuilder().
		AddOp(txscript.OP_RETURN).
		AddData(opReturnData).
		Script()
	if err != nil {
		return nil, fmt.Errorf("failed to build OP_RETURN script: %w", err)
	}

	// Get UTXOs
	utxos, err := ba.client.GetUnspent(ba.address)
	if err != nil {
		return nil, fmt.Errorf("failed to get UTXOs: %w", err)
	}

	if len(utxos) == 0 {
		return nil, fmt.Errorf("no UTXOs available")
	}

	// Build transaction
	tx, err := ba.buildTransaction(utxos, opReturnScript)
	if err != nil {
		return nil, fmt.Errorf("failed to build transaction: %w", err)
	}

	// Sign transaction
	signedTx, err := ba.signTransaction(tx)
	if err != nil {
		return nil, fmt.Errorf("failed to sign transaction: %w", err)
	}

	// Broadcast transaction
	txid, err := ba.client.SendRawTransaction(signedTx)
	if err != nil {
		return nil, fmt.Errorf("failed to broadcast transaction: %w", err)
	}

	log.Printf("Anchored Merkle root %x to Bitcoin tx %s", merkleRoot, txid.String())

	return &AnchorRecord{
		MerkleRoot:   merkleRoot,
		Timestamp:    time.Now().Unix(),
		TxID:         txid.String(),
		OPReturnData: opReturnData,
	}, nil
}

// buildOPReturnData constructs the OP_RETURN payload
func buildOPReturnData(merkleRoot [32]byte) []byte {
	// Protocol identifier: HSK
	protocol := []byte("HSK")
	
	// Version: 0x0001
	version := []byte{0x00, 0x01}
	
	// Combine: HSK + version + Merkle root
	data := make([]byte, 0, 36)
	data = append(data, protocol...)
	data = append(data, version...)
	data = append(data, merkleRoot[:]...)
	
	return data
}

// buildTransaction constructs the Bitcoin transaction
func (ba *BitcoinAnchor) buildTransaction(
	utxos []*UTXO,
	opReturnScript []byte,
) (*wire.MsgTx, error) {
	tx := wire.NewMsgTx(wire.TxVersion)

	// Add OP_RETURN output
	opReturnOutput := &wire.TxOut{
		Value:    0,
		PkScript: opReturnScript,
	}
	tx.AddTxOut(opReturnOutput)

	// Calculate fee (assume 1 sat/vbyte)
	// Typical OP_RETURN tx: ~150 bytes
	fee := int64(150)

	// Add inputs until we have enough for fee
	var totalInput int64
	for _, utxo := range utxos {
		if totalInput >= fee {
			break
		}

		txid, err := chainhash.NewHashFromStr(utxo.TxID)
		if err != nil {
			continue
		}

		input := &wire.TxIn{
			PreviousOutPoint: wire.OutPoint{
				Hash:  *txid,
				Index: utxo.Vout,
			},
		}
		tx.AddTxIn(input)
		totalInput += utxo.Amount
	}

	if totalInput < fee {
		return nil, fmt.Errorf("insufficient funds for fee")
	}

	// Add change output if necessary
	change := totalInput - fee
	if change > 546 { // Dust limit
		changeScript, err := txscript.PayToAddrScript(ba.address)
		if err != nil {
			return nil, err
		}
		changeOutput := &wire.TxOut{
			Value:    change,
			PkScript: changeScript,
		}
		tx.AddTxOut(changeOutput)
	}

	return tx, nil
}

// signTransaction signs all inputs
func (ba *BitcoinAnchor) signTransaction(tx *wire.MsgTx) (*wire.MsgTx, error) {
	for i := range tx.TxIn {
		// Get the previous output script
		prevOutputScript, err := txscript.PayToAddrScript(ba.address)
		if err != nil {
			return nil, err
		}

		// Create signature hash
		sigHash, err := txscript.CalcSignatureHash(
			prevOutputScript,
			txscript.SigHashAll,
			tx,
			i,
		)
		if err != nil {
			return nil, err
		}

		// Sign
		signature, err := ba.privateKey.Sign(sigHash[:])
		if err != nil {
			return nil, err
		}

		// Build script signature
		sigScript, err := txscript.NewScriptBuilder().
			AddData(append(signature.Serialize(), byte(txscript.SigHashAll))).
			AddData(ba.privateKey.PubKey().SerializeCompressed()).
			Script()
		if err != nil {
			return nil, err
		}

		tx.TxIn[i].SignatureScript = sigScript
	}

	return tx, nil
}

// VerifyAnchor verifies that a Merkle root was anchored
func (ba *BitcoinAnchor) VerifyAnchor(txid string) (*AnchorRecord, error) {
	hash, err := chainhash.NewHashFromStr(txid)
	if err != nil {
		return nil, fmt.Errorf("invalid txid: %w", err)
	}

	tx, err := ba.client.GetTransaction(hash)
	if err != nil {
		return nil, fmt.Errorf("failed to get transaction: %w", err)
	}

	if tx.Confirmations < 6 {
		return nil, fmt.Errorf("transaction not confirmed (only %d confirmations)", tx.Confirmations)
	}

	// In a real implementation, would fetch and parse the full transaction
	// to extract the OP_RETURN data and verify the Merkle root

	return &AnchorRecord{
		TxID:        txid,
		BlockHash:   tx.BlockHash,
		BlockHeight: 0, // Would be populated from block info
	}, nil
}

// ParseOPReturn extracts Merkle root from OP_RETURN data
func ParseOPReturn(data []byte) ([32]byte, error) {
	if len(data) < 36 {
		return [32]byte{}, fmt.Errorf("OP_RETURN data too short")
	}

	// Check protocol identifier
	if !bytes.Equal(data[:3], []byte("HSK")) {
		return [32]byte{}, fmt.Errorf("invalid protocol identifier")
	}

	// Check version
	version := uint16(data[3])<<8 | uint16(data[4])
	if version != 1 {
		return [32]byte{}, fmt.Errorf("unsupported version: %d", version)
	}

	// Extract Merkle root
	var merkleRoot [32]byte
	copy(merkleRoot[:], data[5:37])

	return merkleRoot, nil
}

// BatchAnchor anchors multiple Merkle roots in a single transaction
// using a Merkle tree of Merkle roots
func (ba *BitcoinAnchor) BatchAnchor(merkleRoots [][32]byte) (*AnchorRecord, error) {
	if len(merkleRoots) == 0 {
		return nil, fmt.Errorf("no Merkle roots to anchor")
	}
	if len(merkleRoots) > 16 {
		return nil, fmt.Errorf("batch too large (max 16)")
	}

	// Build Merkle tree of Merkle roots
	batchRoot := computeBatchRoot(merkleRoots)

	// Anchor the batch root
	return ba.AnchorMerkleRoot(batchRoot)
}

// computeBatchRoot computes Merkle root of multiple Merkle roots
func computeBatchRoot(roots [][32]byte) [32]byte {
	if len(roots) == 1 {
		return roots[0]
	}

	// Simple implementation - in production, use proper Merkle tree
	h := sha256.New()
	for _, root := range roots {
		h.Write(root[:])
	}
	
	var result [32]byte
	h.Sum(result[:0])
	return result
}

func main() {
	// Example usage
	fmt.Println("HSK Bitcoin Anchor")
	fmt.Println("==================")
	
	// This would be configured with actual credentials
	fmt.Println("\nTo use:")
	fmt.Println("1. Configure Bitcoin RPC connection")
	fmt.Println("2. Provide WIF private key")
	fmt.Println("3. Call AnchorMerkleRoot with your Merkle root")
}

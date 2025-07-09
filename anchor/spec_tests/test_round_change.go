package main

import (
	"encoding/hex"
	"fmt"
	"crypto/sha256"
	"github.com/ssvlabs/ssv-spec/qbft"
	"github.com/ssvlabs/ssv-spec/types"
	"github.com/ssvlabs/ssv-spec/types/testingutils"
)

func main() {
	fmt.Println("Testing round change message creation logic...")

	// Create a 4-member committee (needs 3 for quorum)
	ks := testingutils.Testing4SharesSet()
	
	// Create test state
	state := &qbft.State{
		CommitteeMember:  testingutils.TestingCommitteeMember(ks),
		ID:               testingutils.TestingIdentifier,
		PrepareContainer: qbft.NewMsgContainer(),
		Round:            qbft.FirstRound,
		Height:           qbft.FirstHeight,
	}
	
	// Create signer
	signer := testingutils.TestingOperatorSigner(ks)
	
	// Test data
	testData := []byte{1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9}
	testDataHash := sha256.Sum256(testData)
	
	fmt.Printf("Test data: %s\n", hex.EncodeToString(testData))
	fmt.Printf("Test data hash: %s\n", hex.EncodeToString(testDataHash[:]))
	
	// Create 2 prepare messages (not enough for quorum of 3)
	prepareMsg1 := testingutils.TestingPrepareMessage(ks.OperatorKeys[1], types.OperatorID(1))
	prepareMsg2 := testingutils.TestingPrepareMessage(ks.OperatorKeys[2], types.OperatorID(2))
	
	// Add prepared state
	state.LastPreparedRound = qbft.FirstRound
	state.LastPreparedValue = testData
	
	// Add prepare messages to container
	state.PrepareContainer.AddFirstMsgForSignerAndRound(testingutils.ToProcessingMessage(prepareMsg1))
	state.PrepareContainer.AddFirstMsgForSignerAndRound(testingutils.ToProcessingMessage(prepareMsg2))
	
	// Create round change message
	roundChangeMsg, err := qbft.CreateRoundChange(state, signer, qbft.FirstRound, []byte{})
	if err != nil {
		fmt.Printf("Error creating round change: %v\n", err)
		return
	}
	
	// Decode the message to examine its contents
	msg := &qbft.Message{}
	err = msg.Decode(roundChangeMsg.SSVMessage.Data)
	if err != nil {
		fmt.Printf("Error decoding message: %v\n", err)
		return
	}
	
	fmt.Printf("\nRound change message contents:\n")
	fmt.Printf("MsgType: %d\n", msg.MsgType)
	fmt.Printf("Height: %d\n", msg.Height)
	fmt.Printf("Round: %d\n", msg.Round)
	fmt.Printf("Root: %s\n", hex.EncodeToString(msg.Root[:]))
	fmt.Printf("DataRound: %d\n", msg.DataRound)
	fmt.Printf("RoundChangeJustification length: %d\n", len(msg.RoundChangeJustification))
	
	// Check if root is zero
	var zeroRoot [32]byte
	if msg.Root == zeroRoot {
		fmt.Printf("Root is ZERO HASH (no justification quorum)\n")
	} else {
		fmt.Printf("Root is NOT zero hash\n")
	}
	
	// Check if it matches the test data hash
	if msg.Root == testDataHash {
		fmt.Printf("Root matches test data hash\n")
	} else {
		fmt.Printf("Root does NOT match test data hash\n")
	}
	
	fmt.Printf("\nFullData: %s\n", hex.EncodeToString(roundChangeMsg.FullData))
	
	// Get message root
	msgRoot, err := roundChangeMsg.GetRoot()
	if err != nil {
		fmt.Printf("Error getting message root: %v\n", err)
		return
	}
	fmt.Printf("Message root: %s\n", hex.EncodeToString(msgRoot[:]))
}
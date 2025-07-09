package main

import (
	"fmt"
	"encoding/hex"
	"github.com/ssvlabs/ssv-spec/qbft"
	"github.com/ssvlabs/ssv-spec/types"
	"github.com/ssvlabs/ssv-spec/types/testingutils"
)

func main() {
	ks := testingutils.Testing4SharesSet()
	
	// Create a message with the specific full data from the test
	msg := testingutils.TestingPrepareMessage(ks.OperatorKeys[1], types.OperatorID(1))
	msg.FullData = []byte{1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9}
	fmt.Printf("Original message FullData length: %d\n", len(msg.FullData))
	fmt.Printf("Original message FullData: %s\n", hex.EncodeToString(msg.FullData))
	
	// Marshal with full data
	fullDataMarshaled, err := msg.MarshalSSZ()
	if err != nil {
		panic(err)
	}
	fmt.Printf("Full data marshaled length: %d\n", len(fullDataMarshaled))
	
	// Marshal without full data
	withoutFullData := msg.WithoutFullData()
	fmt.Printf("WithoutFullData FullData length: %d\n", len(withoutFullData.FullData))
	fmt.Printf("WithoutFullData FullData: %s\n", hex.EncodeToString(withoutFullData.FullData))
	
	withoutFullDataMarshaled, err := withoutFullData.MarshalSSZ()
	if err != nil {
		panic(err)
	}
	fmt.Printf("Without full data marshaled length: %d\n", len(withoutFullDataMarshaled))
	
	// Test justifications marshaling
	msgs := []*types.SignedSSVMessage{msg}
	justifications, err := qbft.MarshalJustifications(msgs)
	if err != nil {
		panic(err)
	}
	
	fmt.Printf("Justification marshaled length: %d\n", len(justifications[0]))
	fmt.Printf("Justification matches without full data: %t\n", len(justifications[0]) == len(withoutFullDataMarshaled))
	
	// Print first 100 bytes of each to compare
	fmt.Printf("\nFirst 100 bytes of justification: %s\n", hex.EncodeToString(justifications[0][:100]))
	fmt.Printf("First 100 bytes of without full data: %s\n", hex.EncodeToString(withoutFullDataMarshaled[:100]))
	
	// Check if they are identical
	identical := true
	if len(justifications[0]) == len(withoutFullDataMarshaled) {
		for i := 0; i < len(justifications[0]); i++ {
			if justifications[0][i] != withoutFullDataMarshaled[i] {
				fmt.Printf("Difference at byte %d: %x vs %x\n", i, justifications[0][i], withoutFullDataMarshaled[i])
				identical = false
				break
			}
		}
	} else {
		identical = false
	}
	
	fmt.Printf("Bytes are identical: %t\n", identical)
}
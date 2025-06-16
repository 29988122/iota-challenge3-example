# IOTA Move CTF - Challenge #3

## Goal

The objective is to call the `get_flag` function. This function requires a `Coin` object with a value of **exactly 5**.

## Problem

The contract only provides a `mint_coin` function that creates coins with a value of **2**. We cannot directly create the required coin of value 5.

## Solution

The solution is a straightforward, two-transaction process:

1.  **Transaction 1: Get Raw Materials**
    This initial transaction is simple: it calls the `mint_coin` function three times to create three `Coin` objects, each with a value of 2.
2.  **Transaction 2: The Logic Core (via PTB)**
    I use a `ProgrammableTransactionBuilder` (`ptb2` in the code) to build a sequence of commands that will be executed in order on the network.
    a.  **Define Inputs (`ptb2.input(...)`)**:
        -   First, I declare all the "ingredients" this transaction will need. This includes:
            -   The three `Coin` objects we just minted.
            -   The shared `Counter` object required by `get_flag`.
            -   The number `5`, which is needed for the `split` function.
        -   The PTB assigns an internal reference (like `coin1_arg`, `counter_arg`) to each input.

    b.  **Chain Commands (`ptb2.command(...)`)**:
        -   Next, I add the sequence of operations (commands) to be executed, using the references from the previous step:
            -   `Command 1: merge(coin1_arg, coin2_arg)`: Merges the second coin into the first. `coin1_arg` now logically represents a value of 4.
            -   `Command 2: merge(coin1_arg, coin3_arg)`: Merges the third coin into the first. `coin1_arg` now represents the final merged coin with a value of 6.
            -   `Command 3: split(coin1_arg, 5)`: This command calls the `split` function on our coin of value 6. **Crucially, this command returns a new object**: the `Coin` with a value of 5. The PTB captures this return value and assigns it a new internal reference (`coin_with_5`).
            -   `Command 4: get_flag(counter_arg, coin_with_5)`: Finally, `get_flag` is called with the required `Counter` and the result from the previous `split` command (`coin_with_5`).
    Because all these commands are bundled into a single transaction, the entire sequence either succeeds or fails together. This atomic nature is what makes the Programmable Transaction so powerful and is key to solving this challenge efficiently.
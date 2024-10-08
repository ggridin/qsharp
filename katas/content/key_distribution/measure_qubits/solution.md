If the `bases[i]` array element is `true`, it means that you're choosing the Hadamard basis for this qubit, and thus an $H$ gate needs to be applied. Otherwise, you choose the computational basis and don't need to apply the $H$ gate before measuring the qubit.

Now, the output is expected to be a Boolean array, and thus you need to measure the each qubit and convert this measurement to a `Bool`. 
1. To measure each of the qubits in one operation call, you can use Q# library operation `MeasureEachZ`.
2. To convert these measurement results into a Boolean array, you can use the function `ResultArrayAsBoolArray` that takes an array of `Result` type as an input and returns the required array of `Bool` type.

@[solution]({
    "id": "key_distribution__measure_qubits_solution",
    "codePath": "./Solution.qs"
})

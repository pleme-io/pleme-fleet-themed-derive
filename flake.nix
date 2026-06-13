{
  inputs.substrate.url = "github:pleme-io/substrate";
  outputs = inputs: inputs.substrate.mkRustToolFlake {
    inherit inputs;
    src = ./.;
  };
}

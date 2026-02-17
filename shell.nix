{
	pkgs ? import <nixpkgs> {}
}:

pkgs.mkShell {
	packages = with pkgs; [
		rustup
		openssl
		pkg-config
		gcc
		jq
		curl
	];
}

;; caixa.lisp — pleme-fleet-themed-derive.
;;
;; Consumed by `pleme-doc-gen` for the SDLC pipeline (Cargo.toml +
;; .pleme-io-release.toml + CI shims + nix module trio + flake.nix).
;; Re-emit with `pleme-doc-gen caixa --source caixa.lisp --out .`.

(defcaixa pleme-fleet-themed-derive
  :kind "Biblioteca"
  :ecosystem "rust-single-crate"

  :package {
    :name "pleme-fleet-themed-derive"
    :version "0.1.0"
    :license "MIT"
    :description "#[derive(FleetThemed)] — mechanize ishou_tokens::FleetThemedConfig::from_fleet."
    :repository "https://github.com/pleme-io/pleme-fleet-themed-derive"
    :homepage "https://github.com/pleme-io/pleme-fleet-themed-derive"
    :categories ["development-tools::procedural-macro-helpers"]
    :keywords ["tatara" "macro" "derive" "ishou" "fleet"]}

  :ci-config {:bump {:default-type "patch"} :publish {:no-verify false}}

  :workflows [:auto-release]
  :stacks []
  :depends-on []
  :exposes [:rust-crate]
  :publish-to-git true)

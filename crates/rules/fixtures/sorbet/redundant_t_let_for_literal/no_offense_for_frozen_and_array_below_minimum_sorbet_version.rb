PATTERN = T.let(/foo/.freeze, Regexp)
SHELLS = T.let([:bash, :zsh].freeze, T::Array[Symbol])
NAMES = T.let(["alice", "bob"], T::Array[String])

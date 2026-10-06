if enabled?
  PATTERN = T.let(/foo/.freeze, Regexp)
  NAMES = T.let(["a", "b"].freeze, T::Array[String])
end

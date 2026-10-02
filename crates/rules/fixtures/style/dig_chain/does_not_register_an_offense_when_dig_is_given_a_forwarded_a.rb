def foo(&)
  x.dig(&).dig(:foo)
end

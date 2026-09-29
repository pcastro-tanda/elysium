def test
  ::Kernel.raise
           ^^^^^ Use `fail` instead of `raise` to signal exceptions.
rescue Exception
  ::Kernel.fail
           ^^^^ Use `raise` instead of `fail` to rethrow exceptions.
end

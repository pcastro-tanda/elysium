def test
  ::Kernel.fail
rescue Exception
  ::Kernel.raise
end

class A
  ROOT = __FILE__
  def same
    foo(bar, \
             ^ Redundant line continuation.
        baz)
  end
end

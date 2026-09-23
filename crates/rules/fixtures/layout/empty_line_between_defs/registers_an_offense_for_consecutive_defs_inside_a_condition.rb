if condition
  def foo
    true
  end
  def bar
  ^^^^^^^ Expected 1 empty line between method definitions; found 0.
    true
  end
else
  def foo
    false
  end
end

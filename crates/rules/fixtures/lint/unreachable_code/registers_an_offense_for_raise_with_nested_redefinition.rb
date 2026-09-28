def foo
  def raise; end
end

raise
bar
^^^ Unreachable code detected.

def foo
  def abort; end
end

abort
bar
^^^ Unreachable code detected.

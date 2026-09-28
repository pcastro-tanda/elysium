def foo
  def self.abort; end
end

abort
bar
^^^ Unreachable code detected.

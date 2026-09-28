def foo
  def self.raise; end
end

raise
bar
^^^ Unreachable code detected.

def foo
  def self.exit!; end
end

exit!
bar
^^^ Unreachable code detected.

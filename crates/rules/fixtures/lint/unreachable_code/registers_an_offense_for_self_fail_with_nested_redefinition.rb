def foo
  def self.fail; end
end

fail
bar
^^^ Unreachable code detected.

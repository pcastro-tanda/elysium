def foo
  def self.throw; end
end

throw
bar
^^^ Unreachable code detected.

class Dummy
  def throw; end
end

d = Dummy.new
d.instance_eval do
  Kernel.throw
  foo
  ^^^ Unreachable code detected.
end

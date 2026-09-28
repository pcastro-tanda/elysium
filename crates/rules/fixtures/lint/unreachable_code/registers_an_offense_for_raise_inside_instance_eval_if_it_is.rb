class Dummy
  def raise; end
end

d = Dummy.new
d.instance_eval do
  Kernel.raise
  foo
  ^^^ Unreachable code detected.
end

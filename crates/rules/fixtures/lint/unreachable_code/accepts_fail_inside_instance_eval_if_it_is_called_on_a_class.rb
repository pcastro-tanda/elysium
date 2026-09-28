class Dummy
  def fail; end
end

d = Dummy.new
d.instance_eval do
  Dummy.fail
  foo
end

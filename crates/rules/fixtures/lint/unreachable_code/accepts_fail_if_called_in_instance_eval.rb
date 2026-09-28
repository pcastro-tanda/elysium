class Dummy
  def fail; end
end

d = Dummy.new
d.instance_eval do
  fail
  bar
end

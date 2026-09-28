class Dummy
  def throw; end
end

d = Dummy.new
d.instance_eval do
  throw
  _1
end

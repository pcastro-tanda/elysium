class Dummy
  def fail; end
end

d = Dummy.new
d.instance_eval do
  d2 = Dummy.new
  d2.instance_eval do
    fail
    bar
  end
end

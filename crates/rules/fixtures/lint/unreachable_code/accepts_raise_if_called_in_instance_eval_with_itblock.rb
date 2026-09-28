class Dummy
  def raise; end
end

d = Dummy.new
d.instance_eval do
  raise
  it
end

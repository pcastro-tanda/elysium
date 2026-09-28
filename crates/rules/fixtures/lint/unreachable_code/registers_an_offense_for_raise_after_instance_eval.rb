class Dummy
  def raise; end
end

d = Dummy.new
d.instance_eval do
  raise
  bar
end

raise
bar
^^^ Unreachable code detected.

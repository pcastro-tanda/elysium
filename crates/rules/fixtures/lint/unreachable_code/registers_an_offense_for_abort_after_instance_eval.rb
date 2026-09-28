class Dummy
  def abort; end
end

d = Dummy.new
d.instance_eval do
  abort
  bar
end

abort
bar
^^^ Unreachable code detected.

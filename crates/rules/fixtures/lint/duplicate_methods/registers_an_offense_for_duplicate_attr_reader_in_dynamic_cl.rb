A = Class.new do
  def something
  end
  attr_reader :something
  ^^^^^^^^^^^^^^^^^^^^^^ Method `A#something` is defined at both example.rb:2 and example.rb:4.
end

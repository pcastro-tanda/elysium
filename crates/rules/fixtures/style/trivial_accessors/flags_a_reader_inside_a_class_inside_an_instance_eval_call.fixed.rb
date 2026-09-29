something.instance_eval do
  class << @blah
    begin
      attr_reader :bar
    end
  end
end

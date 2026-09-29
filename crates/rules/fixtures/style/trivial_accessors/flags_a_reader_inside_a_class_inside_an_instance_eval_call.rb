something.instance_eval do
  class << @blah
    begin
      def bar
      ^^^ Use `attr_reader` to define trivial reader methods.
        @bar
      end
    end
  end
end

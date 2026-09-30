define_method :method_name do
  Struct.new(:foo, :bar) do
    String.class_eval do
      [42].tap do |answer|
        foo { bar }
      end
    end
  end
end

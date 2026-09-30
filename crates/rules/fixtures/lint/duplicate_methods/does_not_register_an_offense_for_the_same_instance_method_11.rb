let(:foo_mod) do
  Module.new do
    def name
      'Foo'
    end
  end
end

let(:bar_mod) do
  Module.new do
    def name
      'Bar'
    end
  end
end

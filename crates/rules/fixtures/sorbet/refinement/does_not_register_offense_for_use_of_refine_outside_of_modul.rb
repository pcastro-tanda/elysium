module MyNamespace
  class MyClass
    refine(String) do
      def to_s
        "foo"
      end
    end
  end
end

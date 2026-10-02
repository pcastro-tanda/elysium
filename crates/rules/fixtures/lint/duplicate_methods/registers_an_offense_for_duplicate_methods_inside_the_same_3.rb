let(:klass) do
  Class.new do
    def foo
      1
    end
    def foo
    ^^^^^^^ Method `::Object#foo` is defined at both (string):3 and (string):6.
      2
    end
  end
end

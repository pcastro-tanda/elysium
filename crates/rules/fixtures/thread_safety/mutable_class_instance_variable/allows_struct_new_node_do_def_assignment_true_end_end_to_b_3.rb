class Test
  @var = ::Struct.new(:node) do
    def assignment?
      true
    end
  end
end
class Test
  define_method(:name) do
    @var ||= [1, 2]
  end
end
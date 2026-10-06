class Test
  define_singleton_method(:name) do
    @var ||= [1, 2]
  end
end
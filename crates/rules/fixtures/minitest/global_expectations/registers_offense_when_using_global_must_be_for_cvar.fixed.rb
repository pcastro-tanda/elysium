it 'does something' do
  @@n = do_something
  _(@@n).must_be 42
end

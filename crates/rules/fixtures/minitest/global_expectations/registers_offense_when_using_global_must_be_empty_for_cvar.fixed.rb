it 'does something' do
  @@n = do_something
  _(@@n).must_be_empty 42
end

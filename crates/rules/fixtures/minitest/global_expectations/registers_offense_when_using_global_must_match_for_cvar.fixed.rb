it 'does something' do
  @@n = do_something
  _(@@n).must_match 42
end

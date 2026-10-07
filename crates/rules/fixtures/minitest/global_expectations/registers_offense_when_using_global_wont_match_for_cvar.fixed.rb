it 'does something' do
  @@n = do_something
  _(@@n).wont_match 42
end

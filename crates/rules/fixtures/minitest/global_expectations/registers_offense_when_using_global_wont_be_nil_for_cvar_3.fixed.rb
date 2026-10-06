it 'does something' do
  @@n = do_something
  _(@@n).wont_be_nil 42
end

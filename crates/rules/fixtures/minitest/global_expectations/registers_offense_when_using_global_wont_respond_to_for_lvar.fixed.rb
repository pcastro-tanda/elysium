it 'does something' do
  n = do_something
  _(n).wont_respond_to 42
end

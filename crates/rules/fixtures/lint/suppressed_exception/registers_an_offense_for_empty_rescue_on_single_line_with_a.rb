RSpec.describe Dummy do
  it 'dummy spec' do
    # This rescue is here to ensure the test does not fail because of the `raise`
    expect { begin subject; rescue ActiveRecord::Rollback; end }.not_to(change(Post, :count))
                            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not suppress exceptions.
    # Done
  end
end

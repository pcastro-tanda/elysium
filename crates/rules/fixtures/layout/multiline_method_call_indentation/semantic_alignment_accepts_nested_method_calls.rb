expect { post :action, params: params, format: :json }.to change { Foo.bar }.by(0)
                                                      .and change { Baz.quux }.by(0)
                                                      .and raise_error(StandardError)

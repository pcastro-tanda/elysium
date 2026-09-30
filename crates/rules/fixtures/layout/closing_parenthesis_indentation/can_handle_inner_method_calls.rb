expect(response).to contain_exactly(
                      { a: 1, b: 'x' },
                      { a: 2, b: 'y' }
                    )

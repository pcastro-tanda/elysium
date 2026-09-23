let(:p) { foo }

it { expect(do_something(k: p)).to eq bar }

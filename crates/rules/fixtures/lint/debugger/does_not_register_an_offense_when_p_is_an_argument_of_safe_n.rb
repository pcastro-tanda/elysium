let(:p) { foo }

it { expect(obj&.do_something(p)).to eq bar }

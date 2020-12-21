import BN from 'bn.js'
import { FlowArgs } from '../../Scenario'
import { ValidatorCountProposalFixture } from '../../fixtures/proposalsModule'
import { assert } from 'chai'
import { FixtureRunner } from '../../Fixture'
import Debugger from 'debug'

export default async function validatorCount({ api, env }: FlowArgs): Promise<void> {
  const debug = Debugger('flow:validatorCountProposal')
  debug('Started')

  // Pre-conditions: members and council
  const council = await api.getCouncil()
  assert(council.length)

  const proposer = council[0].member.toString()

  const validatorCountIncrement: BN = new BN(+env.VALIDATOR_COUNT_INCREMENT!)

  const validatorCountProposalFixture: ValidatorCountProposalFixture = new ValidatorCountProposalFixture(
    api,
    proposer,
    validatorCountIncrement
  )
  await new FixtureRunner(validatorCountProposalFixture).run()

  debug('Done')
}

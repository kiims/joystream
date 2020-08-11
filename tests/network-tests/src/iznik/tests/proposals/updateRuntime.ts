import { KeyringPair } from '@polkadot/keyring/types'
import { initConfig } from '../../utils/config'
import { Keyring, WsProvider } from '@polkadot/api'
import BN from 'bn.js'
import { setTestTimeout } from '../../utils/setTestTimeout'
import tap from 'tap'
import { registerJoystreamTypes } from '@nicaea/types'
import { closeApi } from '../../utils/closeApi'
import { ApiWrapper } from '../../utils/apiWrapper'
import { Utils } from '../../utils/utils'
import { BuyMembershipHappyCaseFixture } from '../fixtures/membershipModule'
import { UpdateRuntimeFixture } from '../fixtures/proposalsModule'
import { PaidTermId } from '@nicaea/types/members'
import { CouncilElectionHappyCaseFixture } from '../fixtures/councilElectionHappyCase'
import { DbService } from '../../services/dbService'

tap.mocha.describe('Update runtime scenario', async () => {
  initConfig()
  registerJoystreamTypes()

  const nodeUrl: string = process.env.NODE_URL!
  const sudoUri: string = process.env.SUDO_ACCOUNT_URI!
  const keyring = new Keyring({ type: 'sr25519' })
  const provider = new WsProvider(nodeUrl)
  const apiWrapper: ApiWrapper = await ApiWrapper.create(provider)
  const sudo: KeyringPair = keyring.addFromUri(sudoUri)
  const db: DbService = DbService.getInstance()

  const N: number = +process.env.MEMBERSHIP_CREATION_N!
  let m1KeyPairs: KeyringPair[] = Utils.createKeyPairs(keyring, N)
  let m2KeyPairs: KeyringPair[] = Utils.createKeyPairs(keyring, N)

  const paidTerms: PaidTermId = new PaidTermId(+process.env.MEMBERSHIP_PAID_TERMS!)
  const K: number = +process.env.COUNCIL_ELECTION_K!
  const greaterStake: BN = new BN(+process.env.COUNCIL_STAKE_GREATER_AMOUNT!)
  const lesserStake: BN = new BN(+process.env.COUNCIL_STAKE_LESSER_AMOUNT!)
  const durationInBlocks = 54

  setTestTimeout(apiWrapper, durationInBlocks)

  if (db.hasCouncil()) {
    m1KeyPairs = db.getMembers()
    m2KeyPairs = db.getCouncil()
  } else {
    const councilElectionHappyCaseFixture = new CouncilElectionHappyCaseFixture(
      apiWrapper,
      sudo,
      m1KeyPairs,
      m2KeyPairs,
      paidTerms,
      K,
      greaterStake,
      lesserStake
    )
    councilElectionHappyCaseFixture.runner(false)
  }

  const updateRuntimeFixture: UpdateRuntimeFixture = new UpdateRuntimeFixture(apiWrapper, m1KeyPairs, m2KeyPairs, sudo)
  tap.test('Upgrade runtime', async () => updateRuntimeFixture.runner(false))

  const thirdMemberSetFixture: BuyMembershipHappyCaseFixture = new BuyMembershipHappyCaseFixture(
    apiWrapper,
    sudo,
    Utils.createKeyPairs(keyring, N),
    paidTerms
  )
  tap.test('Creating third set of members', async () => thirdMemberSetFixture.runner(false))

  closeApi(apiWrapper)
})

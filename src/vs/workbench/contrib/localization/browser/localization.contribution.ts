/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { localize, localize2 } from '../../../../nls.js';
import { Action2, registerAction2 } from '../../../../platform/actions/common/actions.js';
import { ICommandService } from '../../../../platform/commands/common/commands.js';
import { ServicesAccessor } from '../../../../platform/instantiation/common/instantiation.js';
import { ILanguagePackItem, ILanguagePackService } from '../../../../platform/languagePacks/common/languagePacks.js';
import { IQuickPickItem, IQuickInputService } from '../../../../platform/quickinput/common/quickInput.js';
import { LANGUAGE_DEFAULT, Language } from '../../../../base/common/platform.js';
import { ILocaleService } from '../../../services/localization/common/locale.js';

registerAction2(
	class ConfigureDisplayLanguageAction extends Action2 {
		constructor() {
			super({
				id: 'workbench.action.configureLocale',
				title: localize2('configureLocale', 'Configure Display Language'),
				category: 'Preferences',
				f1: true
			});
		}

		async run(accessor: ServicesAccessor): Promise<void> {
			const languagePackService = accessor.get(ILanguagePackService);
			const quickInputService = accessor.get(IQuickInputService);
			const localeService = accessor.get(ILocaleService);
			const commandService = accessor.get(ICommandService);

			const installedLanguages = await languagePackService.getInstalledLanguages();

			const currentLanguage = Language.value();

			const items: (IQuickPickItem & { locale?: string; extensionId?: string })[] = [
				{
					id: 'en',
					label: 'English',
					description: currentLanguage === LANGUAGE_DEFAULT ? localize('activeLanguage', 'Current') : undefined
				}
			];

			for (const lang of installedLanguages) {
				if (lang.id === 'en') {
					continue;
				}
				items.push({
					id: lang.id,
					label: lang.label ?? lang.id ?? '',
					description: lang.id === currentLanguage ? localize('activeLanguage', 'Current') : lang.description,
					locale: lang.id,
					extensionId: lang.extensionId
				});
			}

			items.push({ type: 'separator' } as unknown as IQuickPickItem);

			items.push({
				id: 'install',
				label: localize('installAdditionalLanguages', 'Install Additional Languages...')
			});

			const pick = await quickInputService.pick(items, {
				placeHolder: localize('chooseDisplayLanguage', 'Select Display Language')
			});

			if (!pick) {
				return;
			}

			if (pick.id === 'install') {
				await commandService.executeCommand('workbench.extensions.search', '@category:"language packs"');
				return;
			}

			const selectedLocale = pick.id;
			if (!selectedLocale) {
				return;
			}

			if (selectedLocale === 'en') {
				await localeService.clearLocalePreference();
			} else {
				const languagePackItem: ILanguagePackItem = {
					id: selectedLocale,
					label: pick.label,
					extensionId: (pick as { extensionId?: string }).extensionId
				};
				await localeService.setLocale(languagePackItem);
			}
		}
	}
);
